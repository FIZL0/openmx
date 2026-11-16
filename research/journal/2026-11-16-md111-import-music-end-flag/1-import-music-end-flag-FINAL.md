# 1 — import music end flag (FINAL)

Task MD111. 2026-11-16.

## What I did

- Cause: openmx B1 (3f380b287) made stereo `.vig` reads (`Clip::vig`, `Adpcm`) stop at the end-flagged
  block pair. openmx-mods' `tools/import-music` compiles openmx's audio.rs in, so its reads lost the one
  filler pair (28 frames) MX vs. ATV's songs carry after the flag. Its tests still expected the old read
  and a `todo` line about the filler.
- Fix, all in openmx-mods (branch task/MD111): `ReadBack` gains `pairs` (block pairs inside the header's
  size). `problems()` no longer reports pairs past the end flag; it now reports a read that stops
  anywhere but the end flag. Tests `built.rs` and `disc.rs` expect frames up to the flag and no filler
  `todo`. `notes/music.md` and `notes/openmx-asks.md` (MD7) say B1 closed it.
- The full gate then showed a second red test, `tools/accuracy` `the_shell_loads_a_generated_mod_as_openmx_does`:
  openmx S320 (cdca4ffff) answers the field's last sample row and column, so its 2 "strip" points now
  equal the harness. It expects 0 now; `notes/openmx-asks.md` marks that ask done.
- Landed as openmx-mods 946c449, pushed to its main (a fast-forward of a87ef11).
- No openmx change. The task line's `./check.sh` is openmx-mods' gate, which already exists.
- Took nothing from opentennis: it has no music-pack tool.

## What the original does

Not touched: this is a mod tool following openmx's reader.

## How it was proved

- openmx-mods `cargo test -p import-music`: 8/8 in `built.rs` against today's openmx.
- `disc.rs` (ignored by default), run with `DISCX_ISO` on both disc images: all 20 MX vs. ATV songs have
  exactly 1 pair after the flag, all 22 Untamed songs 0, every read stops at the flag, `todo` is the T7
  line alone, modcheck proves the packs byte for byte.
- openmx-mods `./check.sh` with `openmx` linked to this checkout (main a143a26a5): 153 passed, 0 failed.

## Not verified / not 1:1

- That the PS2's SPU stops at the end flag and never plays the filler: taken from B1, not re-checked
  here (no capture of the game's music end).
- The built packs under `out/mods/music/` were not rebuilt; their `mod.json` `todo` keeps the old filler
  line until the next `import-music` run (out/ is not in git).
- openmx-mods `notes/accuracy.md`'s per-map table still shows the pre-S320 "14 in the last strip" counts:
  regenerating it needs a shell run per map on the discs, outside this task. Its † note says so.

## Next

none
