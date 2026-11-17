# F31b5 — whole frame closed

2026-11-17. Task: `./kit plan F31b5`.

F31b5 was reopened after its last follow-ups (F31b5q..v, and F67 for the wreck posts' camera) landed.
With every switch on, C15 ride-2 and flip-1 now replay bit-exact on every row with the interpreter
running 0 instructions in any frame; flip-1 wrecks on row 125 and is put back on row 193. The other
five riders' PreProcess was shown not to touch the bike by F31b5v. This session ran both replays,
proved they were not skipped, and took the `#[ignore]` off the ride test so the gate runs both.
Earlier entries: `../2026-11-04-f31b5-bike-on-quad-steps/` … `../2026-11-14-f31b5-whole-frame-in-rust/`.
