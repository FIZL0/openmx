# V1 — the first draw

2026-10-15. Task: `./kit plan V1`.

One frame of a track with a bike standing on it, read from the user's disc at runtime and rasterised by
`crates/game-render` — four binary formats (`.psg` geometry, `.pst` textures, `.trn` terrain, plus D5's two
container layers) and four text ones (`.scn`, `.psm`, `.sub`, `cpresets.ini`), 0.07 s from ISO to PNG,
compared by eye against the live game at the same start gate on the same track.

The entry is worth reading for three things: what a `.trn` patch offset is measured from (and the test that
tells you when you have it wrong), why the terrain's height band is *fitted* here rather than read (it is
not in any file I could find, and that is the biggest unproven thing in the task), and that a bike on this
disc is a shared chassis plus a parts list plus a repaint, not a model.
