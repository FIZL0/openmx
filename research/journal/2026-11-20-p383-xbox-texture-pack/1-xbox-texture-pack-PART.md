# 1 — xbox texture pack (PART)

Task P383. 2026-11-20.

## What I did

- **The Xbox disc image is not on this machine.** `MX Unleashed (USA) (Xbox).iso` is gone from the repo root
  (only the PS2 image is there), and is not in `~/Downloads` (its `MX Unleashed (USA).7z` is the PS2 one,
  2.2 GB extracted on 10-15), `context/`, the other slots, or `/mnt/dev`. No unpacked Xbox `.res` tree
  either. It was likely lost with the stuck 11-01 rebase in the main checkout. So the survey and the pack
  itself could not be made; everything that does not need the disc is done.
- `tools/xbox_textures.py pack TEXTURES_SRC XBOX_DIR OUT [--keep NAME ...]`: for each PS2 texture in
  `openmx --dump-textures`'s output (`<archive>/<member>-<key>.png`, so the key is the port's own
  `game_render::replace::key` of the PS2 decode), the Xbox `.dxt` of the same archive and member
  (case-blind), decoded by the script's existing decoder. Kept when it has more pixels, or the user named it
  (`--keep`, or the `KEEP` list in the script); written as `OUT/textures/<member>-<PS2 key>.png` with a
  `"kind": "textures"` `mod.json`. Every pair goes in `OUT/survey.tsv`. Pack mode needs numpy only: PIL is
  not installed for the system python here, so PNGs are read (IHDR) and written with zlib; the old
  side-by-side mode imports PIL on its own.
- `tools/upscale_mods.py --disc PACK [--src mods/textures-src] [--xbox mods/xbox-textures] [--check]`:
  upscales the disc's textures headless into a texture pack (it was a by-hand chaiNNer step before); with
  `--xbox` the Xbox pack's picture of a key is the source instead of the dump's PNG. Same `sources.json`
  staleness as the mods' upscales, so a source switch from PS2 to Xbox redoes it.
- `crates/openmx/src/textures.rs` needed no change: a `"kind": "textures"` pack in `mods/xbox-textures/`
  already stacks on top.
- Recipe in `research/mxu-xbox.md` "The texture pack (P383)".
- Spec review fixes: `--disc --check` writes nothing; a rerun of `pack` drops pictures it no longer keeps;
  the docstring says which pack is on top when both name a key (`upscaled-disc` sorts over `xbox-textures`).
- opentennis: nothing to take (no other-platform texture source there).

## What the original does

Not a simulation task: the Xbox disc is only a texture source; the PS2 game is unchanged with the pack off.

## How it was proved

`crates/openmx/tests/xbox_textures.rs`:
- `a_pack_png_keyed_to_free06_grass_replaces_it` reads `Tracks\free06.res` off the PS2 disc (ran, not
  skipped): free06_grass is 128x128 there; a 256x256 PNG named by its key in a texture pack is drawn in its
  place, and the disc's again with the pack off.
- `the_pack_keeps_the_bigger_xbox_picture_under_the_ps2_key`: hand-made PS2 dumps and Xbox `.dxt`s (format
  0x37) through `xbox_textures.py pack` — the bigger Xbox grass is kept under the PS2 key, the same-size rock
  only with `--keep`, the store's pack checks pass, the shell draws it; `upscale_mods.py --disc --xbox
  --check` treats the Xbox picture as grass's source (sources.json with its SHA-1 is done, with the PS2
  PNG's is stale).

## Not verified / not 1:1

- **The survey of every .res, and the pack**: not done — the Xbox iso is missing (above). No
  `mods/xbox-textures/` exists.
- **free06 with the pack on and off, by eye**: not done for the same reason; the free06 test uses a
  stand-in 256x256 picture, not the Xbox grass.
- **Xbox decoder on real files** in pack mode: only the hand-made 0x37 case runs in the test; DXT1/DXT5 and
  0x38 are the decoder already checked by eye on 10-31 (research/mxu-xbox.md), unchanged here.
- **Name matching against the real Xbox tree**: assumes `xbox_res.py`'s unpacked member paths equal the PS2
  member paths (case aside), as the 10-31 notes found for free06 and the 250SX. Other archives unchecked.
- **Terrain ground pages and `.atm` animations**: not matched (the Xbox `.dx.trn` is undecoded).
- **"Or the user judges it better"**: the `KEEP` list is empty; it needs the user to look at the survey.

## Next

The user puts `MX Unleashed (USA) (Xbox).iso` back at the repo root; then follow research/mxu-xbox.md
"The texture pack (P383)" steps 1–3, take free06 shots with the pack on and off, and fill `KEEP` from the
user's picks.
