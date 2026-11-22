# MX Unleashed, Xbox (USA): a research reference

User, 2026-10-31: the Xbox build has better textures in places. Rip those into the texture replacement
path (P383). The PS2 disc stays the game; the Xbox disc is only a texture source.

## Where it is

`MX Unleashed (USA) (Xbox).iso` at the repo root beside the PS2 image (git-ignored by `*.iso`), 1.44 GB,
a trimmed XISO.

## Reading it

- `tools/xbox_xiso.py ISO ls|x OUT [prefix]`: the Xbox disc filesystem (XDVDFS). The volume descriptor
  `MICROSOFT*XBOX*MEDIA` is at partition + 0x10000 (partition 0 for a trimmed image); directories are
  binary trees of `{u16 left, u16 right (dwords), u32 sector, u32 size, u8 attr, u8 namelen, name}`.
- 962 files as a plain tree (`Tracks/free06.res`, `Bike/250SX.res`, `Rider/…`, `UI/…`, `Aud/…`), where
  the PS2 puts everything in one `MX.PSR`. `tools/ps2_psr.py` pulls a member out of the PS2 image.
- `tools/xbox_res.py FILE ls|x OUT`: the `.res` container, the **same** version-3 layout as the PS2's
  inner `.res` (`crates/game-sim/src/archive.rs` `Res`): zlib blocks of 24,576 bytes, each followed by
  0xFF, and a name/offset/size index.
- `default.xbe`: stripped retail x86. 3 RTTI names, all from the C++ runtime, and 2 `.cpp` strings. No
  reading aid.

## Textures: `.dxt` in place of `.pst`

Header `u32 2, u32 format`, then per format:

| format | what | layout after the format word |
|---|---|---|
| 0x37, 0x38 | 8-bit palette (0x38 keeps alpha) | u32 256, 256 × BGRA, then u32 mips, u32 W, u32 H, mips |
| 0x26 | DXT1 | u32 0, u32 mips, W, H, mips |
| 0x36 | DXT5 (the 512×512 shadow maps among others) | as 0x26 |

Mips are smallest first, each `{u32 w, u32 h, u32 bytes, data}`. Palette pixels are **swizzled** (the
Xbox's Morton order, x bit first); DXT blocks are linear. `tools/xbox_textures.py` decodes both discs'
textures and lays each pair side by side; checked by eye on free06 and the 250SX in every format.

## What differs from the PS2 (free06 and Bike/250SX, checked 2026-10-31)

- **Textures**: 108 pairs, 106 the same size. Most Xbox ones are DXT, so slightly blockier gradients at
  the same resolution. `free06_grass` and `mill_siding` are **256×256 on Xbox, 128×128 on PS2**,
  redrawn with more detail (more planks in the siding), not upscaled. Other tracks, bikes, riders and UI
  are not surveyed yet.
- **Sim data** is the same: all `.mot` (13), `.col` (112), `.hit`, `.dat`, and 7 of 11 `.phy` are
  byte-identical. The four 250 race/pro `.phy` differ in one line: `MaxPitchCommand` 40 (PS2) vs 65
  (Xbox).
- **Cameras**: the `appstart\*cpresets.ini` files are retuned (e.g. cpresets Preset FOV 85 → 90, AngleX
  65 → 50; the biplane FollowingDistance 13 → 28), and `fppreset.ini` adds widescreen presets.
- Terrain: `.dx.trn` 3.1 MB vs `.ps2.trn` 2.0 MB for free06. Not decoded.

## The texture pack (P383)

1. `tools/xbox_xiso.py XBOX.iso x X` and, for each `.res`, `tools/xbox_res.py X/<dir>/<name>.res x XU/<dir>/<name>`
   (the unpacked tree mirrors `openmx --dump-textures`'s `<archive>/<member>` layout).
2. `openmx --dump-textures` (PS2 disc → `mods/textures-src/`).
3. `tools/xbox_textures.py pack mods/textures-src XU mods/xbox-textures [--keep NAME ...]`: every pair goes in
   `mods/xbox-textures/survey.tsv`; an Xbox picture with more pixels than its PS2 twin, or one named in
   `--keep` / the script's `KEEP`, is written as `textures/<member>-<PS2 key>.png` in a `"kind": "textures"`
   pack. Needs numpy only (no PIL). Terrain pages (`.ps2.trn` vs `.dx.trn`) and `.atm` are not matched.
4. Upscaled with the Xbox picture as the source: `tools/upscale_mods.py MODEL --disc mods/upscaled-disc --xbox
   mods/xbox-textures`.

The whole-disc survey (2026-11-21): 520 `.res` unpack to 9,565 `.dxt` (formats 0x26 DXT1 5124, 0x36
DXT5 3007, 0x37 1374, 0x38 36, 0x32 DXT3 24); the PS2 dump has 10,786. 9,424 pairs: **376 Xbox pictures
have more pixels** (274 keys, a key shared across archives always got the same picture), 8,998 the same
size, 50 fewer. The bigger ones: 255 track previews (`UI/trackpreview`, 256 → 512), 27 bike chrome in `UI/Bike`
(64 → 256), the `free06`/`free09`/`nat*` mill and wood pictures, `free06_grass` (128 → 256), one
`score_tower` per `sx*` track. 1,258 PS2 textures have no Xbox twin. Format 0x32 (23 `sx*/tex/score_tower4`,
`nat10/tex/power_line`) is DXT3: the DXT5 layout, but sixteen 4-bit alphas; decoded so, score_tower4's
alpha correlates with the PS2 picture's at 0.9999 (as DXT5, 0.92). `pack` reads the sizes from the headers
and decodes only the kept pictures (33 s for the disc).
