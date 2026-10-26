# Texture replacements

Upscaled or edited textures are read when a ride loads, from texture packs and from
`mods/texture-replacements/` in the mods folder beside your settings (or the one `--mods DIR` names). None of
it is part of the repository, and none of it is shipped: like everything from the disc, it is made on your
machine. Each world texture is taken from the first match:

0. **Texture packs**: mod folders under `mods/` whose `mod.json` says `"kind": "textures"`, holding key-named
   files anywhere under their `textures/`. They stack: by default a pack nobody has ordered is on top, and
   settings.txt's `texture_packs = a, b` (top first) and `texture_packs_off = c` (folder names under `mods/`,
   such as `hd_ground` or `packs/hd_bikes`) set the order and turn packs off.
1. **Key-named files** (`<name>-<key>.png`, in any subfolder or flat): your own textures, edited or upscaled,
   at any size and with straight (0–255) alpha. A file is matched by the part of its name after the last `-`.
2. **A PCSX2 texture pack**: the hash-named PNGs PCSX2 dumps and loads (`<tex>-<clut>-<bits>.png`), put
   straight in `mods/texture-replacements/` (not a subfolder), so a pack made for MX Unleashed in PCSX2 works
   as it is. Their alpha is the GS's (128 is opaque), as PCSX2 writes them. A far-off model can draw from a
   smaller mip as its base, and PCSX2 names that upload separately; the largest one the pack has is drawn.
   The `-mipN` files are not read (mips are rebuilt from the picture), and neither are the loading screens,
   the HUD or the riders' number plates, which the game paints at run time.
3. **A mod's own upscales**: key-named files under `textures/upscaled/` in a track, trick or other mod's
   folder, for the textures the mod itself carries (`tools/upscale_mods.py` makes them, below).
4. The disc's, or the mod's own.

```
MX Unleashed (USA).iso
settings.txt
mods/textures-src/...                        (written by --dump-textures)
mods/texture-replacements/...                (yours: upscaled/edited key-named files)
mods/texture-replacements/<hash>-<hash>-<bits>.png   (a PCSX2 pack, at the top level)
mods/<pack>/mod.json + textures/             (a texture pack: the same key-named files, shareable)
mods/tracks/<id>/textures/upscaled/...       (a track mod's own upscales)
```

A pack's `mod.json` needs only its kind:

```json
{ "kind": "textures", "name": "HD ground" }
```

**What is replaced**: the 3D world's textures — tracks' props and ground layers, their detail maps and painted
roads, a mod map's ground pages, the start zones, tire treads, bikes, riders, the biplane and the other
machines. Each is drawn at the replacement's size with the disc texture's own UVs, so a tiled ground or a tiled
face keeps its 1× tiling, and its mips are rebuilt from the replacement. A map's ground pages share one texture
array: if a pack replaces only some of them, the rest are resampled to its size. **What is not**: the HUD's and
the menus' sprites and fonts, the particle sheets and the ground's shadow tiles, which are cut by pixel
rectangles or joined pixel row to pixel row, and a combined world's baked ground pages, which are baked from
the (replaced) layers they are made of.

**Making your own:** `openmx --dump-textures` writes every disc texture, at its native size, to
`mods/textures-src/` in readable folders (`<archive>/<member>-<key>.png`, a terrain's pages as
`<archive>/<terrain>/page_NN-<key>.png`). Edit or upscale those and put the results in
`mods/texture-replacements/` (or a pack's `textures/`) under the same file names. A flat folder is fine,
because only the key matters. `openmx --dump-textures OUT --from MOD` does the same for one mod's folder.

The key is a hash of the decoded picture (its size and its RGBA), so it is the same on every run and every
build, and a texture that two archives carry is one key: one file replaces both.

The Graphics tab's **Upscaled Textures** row is the master switch for all of them (`--textures original` for one
ride; the Original Game switch turns it off too). Off, every texture is the disc's, byte for byte. Both
renderers draw the replacements; the software one is slower with big ones.

To upscale them all at once, use the chaiNNer chain in `tools/chainner/upscale.chn` (chaiNNer 0.25.1). It reads
every PNG under `mods/textures-src/`, runs a 4× model and writes the results to `mods/texture-replacements/`.
The file has no folders or model set, so after opening it in chaiNNer:

- Set *Load Images* to your `mods/textures-src` and *Save Image* to your `mods/texture-replacements`.
- Set *Load Model* to your own upscaling model. The chain was made for 4xHDcube4 (a paid model, not included),
  but any 4× ESRGAN-style model works.

**Mods** carry their own PS2 textures (`.pst`, `.atm`, and a mod map's `.trn` pages).
`python3 tools/upscale_mods.py MODEL.pth` upscales every mod under `mods/` at once with the same chain, run
headless (no GUI, no paths to set), into each mod's `textures/upscaled/`. It decodes the textures with the
port itself (`--openmx`, default `target/release/openmx`, so `cargo build --release -p openmx` first). It
skips textures already done (`textures/upscaled/sources.json`), so rerun it after adding or rebuilding mods;
`--check` lists what is missing or stale and `--prune` deletes upscales nothing names any more.
`"upscale": false` in a mod's `mod.json` skips it.

The ride's log line `textures: N from P texture packs, M key-named in mods/texture-replacements, C PCSX2-named,
U mods' own upscales` shows what was found. A changed file is drawn from the next ride.
