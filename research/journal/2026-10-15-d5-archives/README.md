# D5 — the archives

2026-10-15. Task: `./kit plan D5`.

Two formats, both cracked and both implemented in `crates/game-sim/src/archive.rs` with a hand-written
inflate: `MX.PSR`, the one 1.4 GB sector-aligned container that is the whole game, and `.res`, the
compressed container that nearly every member of it turns out to be. `tests/archive.rs` extracts *every*
member of a `.res` out of `MX.PSR` and checks the byte accounting of both layers.

What that bought immediately: the track (`tracks\sx01\sx01.scn`, its terrain and 69 collision meshes) and
the bike's physics parameters (`bike\race250a.phy`) are now readable, and the physics parameters are
**plain text** — a full tire, strut, engine and per-surface friction model as an INI file. The S3 work is
now about matching the code that reads those numbers, not about recovering the numbers.
