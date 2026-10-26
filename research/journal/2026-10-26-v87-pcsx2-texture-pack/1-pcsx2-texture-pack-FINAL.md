# 1 — pcsx2 texture pack (FINAL)

Task V87. 2026-10-26.

## What I did

- **Ported opentennis's `hst-data/src/texhash.rs`** to `crates/game-render/src/texhash.rs` (the brief's
  path names an opentennis crate this repo does not have; game-render is where `.pst` lives). The GS half
  is unchanged: `GsTex`, the PSMT8/PSMT4/CT32 block swizzle (`in_block`, with opentennis's GS-manual
  test), `level_bytes`, `pcsx2_name` (XXH3-64 of the texture as it sits in GS memory, XXH3-64 of the
  palette, `PSM | TW<<6 | TH<<10`). Left out: opentennis's disc walk, `Overrides`, `disc_prints`,
  `is_disc_copy` and PNG IO, which are its game's formats or which openmx's `textures.rs` already has.
  Added: `Pst`, a `.pst`/`.atm` read as the GS gets it (PSMT8 with the palette in the logical order
  `pst::unswizzle` decodes by, PSMT4 with its 16 entries in file order, raw alpha, levels largest first,
  TW/TH the next power of two); `pack()` (the hash-named PNGs at a folder's top level, not `-mipN`);
  `expand_alpha` (opentennis's 0x80 → 255).
- **`replace::Source` takes the PCSX2 names lazily**: `swap_file(t, file)` / `replace::parse(file)` hash
  the `.pst` only when the shell asks (only when a PCSX2 pack was found). The world's materials, the
  splat's layers, the ground detail map, the ground meshes' paint, the start zones and the tire treads
  now pass their file. `swap(t)` (no file: terrain pages, mod textures from other sources) asks with no
  names.
- **`textures.rs` layer 2**: `Found` keeps three maps (packs + own key-named, PCSX2 by name, upscales by
  key) and `Found::file` takes them in that order; a PCSX2 picture's alpha is doubled. The startup line
  says `C PCSX2-named` (opentennis's word). docs/TEXTURES.md says how to use it.
- **Names generated**: every run of levels `a..=b` (opentennis's `pcsx2_names`), ordered base first and
  longest chain first, so the largest picture a pack has wins.

## What the original does

It uploads a `.pst` as PSMT8 (256-colour, CSM1 palette — the file stores the palette in the GS's
swizzled order) or PSMT4 (16-colour), with its mip chain. A model far off is drawn with **a smaller level
as TEX0's base** (and the rest of the chain after it), so PCSX2 names that upload by the run starting at
that level: 37 of the dump's 94 disc textures were such. That is why a first-level-only cut of the names
found 57 of 108; all runs find 94.

## How it was proved

A real PCSX2 2.9.127 dump of this game: copy 2's ini set `DumpReplaceableTextures` and
`DumpReplaceableMipmaps` (Tools > Texture Replacement > Dump Textures / Mipmaps), loaded
`context/v77/free06-bike.p2s`, rode 3 s, paused and unpaused; 143 files (108 names + their `-mipN`)
copied to `context/v87/free06-dump/` (game data, git-ignored). `crates/openmx/tests/texhash.rs` names
every `.pst`/`.atm` on the disc and checks:

- every dumped name whose palette is a disc `.pst`'s palette is one of the disc's names (none wrong);
- 94 named, of them 32 PSMT4, 13 mipped (with `-mipN` files), 37 based on a smaller level — each count
  read by raising the assert past it once (a pass would hide a skip);
- each one's PNG is pixel-for-pixel the disc level it starts at, through the palette with raw alpha;
- `textures::scan` finds a base-level file for its disc texture and `pcsx2_texture` gives it at full
  size with the alpha doubled.

So mips, the CLUT's layout (8-bit swizzled and 4-bit plain) and 4-bit textures all hash as opentennis's
code does; nothing in the GS half needed a change. The ini was set back to no dumping afterwards.

## Not verified / not 1:1

- **14 of the 108 dumped names are not named**, none with a palette any disc `.pst` has: about six riders'
  number plates (painted at run time), the "Loading" text and the two halves of the MX Unleashed logo
  (presumably the loading screens, `UI\*.lsi`, which are not `.pst`s — not opened), three rounded
  menu/HUD shapes of unproven source, and two 1024x1024 32-bit render targets. The world renderer does not
  swap the HUD, menus or loading screens anyway (V84).
- **The pack's picture was not looked at in a ride**: the test proves the lookup hands back the right
  file and pixels; no ride was drawn with a real PCSX2 pack (none exists here, and an upscaled one would
  need chaiNNer, V84).
- **`-mipN` files are ignored**: the renderer rebuilds mips from the picture. A pack whose only file for
  a texture is a smaller-level upload is drawn from that smaller picture.
- **`.atm` frames after the first**: PCSX2 names each animated frame; the port draws the first frame
  only, so only its name is matched.
- **Terrain `.trn` pages and mod textures** pass no file (`swap(t)`), so a PCSX2 pack cannot replace them;
  free06's ground is splat layers (`.pst`, covered), so no dumped texture was a page. Supercross tracks'
  `.trn` pages were not dumped and are unproven.
- **Only free06 and the bikes/riders in that state** were dumped; other tracks' textures use the same
  two formats, but were not checked one by one.
- **Live reload** stays V86's.

## Next

Done; nothing left for this task.
