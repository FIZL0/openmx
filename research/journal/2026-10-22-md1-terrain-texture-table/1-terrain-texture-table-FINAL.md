# 1 — terrain texture table (FINAL)

Task MD1. 2026-10-22.

## What I did

- Found the parser half of this task already on main: S98 (c42f4b3, landed the same day MD1 was filed)
  taught `track::Terrain::parse` to read the texture count, the offset table and the pages stored
  between the last block and the tree, into `Terrain::textures` (`TerrainTexture` / `TextureLevel`),
  for MXU's own nationals. That is the same layout MX vs. ATV uses, so nothing in the parser had to change.
- Checked that claim on the other games' files with a throwaway probe crate (scratchpad, not committed)
  linking this branch's game-sim: all 56 `.ps2.trn` in openmx-mods' `extracted/mxvatv/Env/*.res`
  (unpacked through `archive::Res`) and all 49 loose ones in `extracted/untamed/Env/` parse.
  MX vs. ATV has v7, 16 / 32 / 48 / 52 / 64 textures, 1 or 2 levels, palette present. Every page is format
  39, one level, 256×256, 65,536 bytes; the header's fourth word is 0x4081c0 or 0x4082d3. `cell()` reads on
  every one of them: 49 hits per terrain on the 1-level ones and 1120 or 800 on the 2-level ones,
  sampling every 37th × 41st sample over 0..5760.
- Added two unit tests in `track.rs`'s own test module, so they run without any disc:
  - `a_terrain_with_pages_before_its_tree_reads_and_hands_them_out`: builds a one-level PLN v7 in the
    test (palette, two blocks, a texture table of two, a 24-byte-header 256×256 format-39 page plus a
    two-level format-40 page, then a root and two leaves, one of them flattened). It checks the header
    fields, that each page's pixels are the right bytes of the file, and that `cell()`'s heights, normals
    and surfaces come out of both leaves, plus `None` for an empty child and for a cell past the leaf's edge.
  - `a_terrain_whose_pages_do_not_reach_its_tree_is_refused`: a table offset that disagrees with where a
    page starts, and a stray byte before the tree (with the header's tree offset moved to match), are both
    refused.
- Doc on `Terrain::textures` now says MX vs. ATV and Untamed carry 16–64 of them.

Decision: the brief asked to "accept blocks_end <= tree_at". S98 made the check stricter instead: the
blocks plus the pages must reach the tree exactly. Every terrain of all three games satisfies that, so I
kept the exact check. A loose `<=` would hide a misread page size. If a mod terrain ever carries real
padding there, relax the check then, with that file as the test.

Nothing from opentennis: it has no heightfield terrain format.

## What the original does

The loader reads the blocks, then a texture count and that many textures, one after another (format
word, level count, then per level width / height / size / unused word and the bytes), then the tree.
It skips the offset table in front of the blocks. S98's journal has the recipe. MD1 adds nothing
read from the decompilation.

## How it was proved

- The generated terrain in the two unit tests above (`./kit check`, game-sim lib).
- Disc terrains unchanged: no line of `Terrain::parse`, `node`, `leaf`, `block` or `cell` changed (the
  only edit to non-test code is a doc comment). The existing disc tests
  (`crates/game-sim/tests/track.rs`: every track's terrain accounts for its blocks and textures, every
  leaf agrees with its bounds, spawn ground, props on the ground) ran here against context/iso, 9 passed.
- The 105 MX vs. ATV / Untamed terrains parse: scratchpad probe, output summarised above. Not a
  committed test, because those discs are not this kit's game data (see below).

## Not verified / not 1:1

- No committed test reads an MX vs. ATV or Untamed terrain. Those files live in the openmx-mods
  checkout, not in this kit's `context/`, and a test here should not depend on another repo's
  extraction. The proof that they parse is the one-off probe run recorded above.
- I have not checked MX vs. ATV's own loader against MXU's in its executable. The claim that it reads
  the same layout rests on all 105 files parsing to the byte and reading sane cells, not on its code.
- I only checked that the pages' pixels are 8-bit indices into the header's 1024-byte palette by their
  sizes (65,536 = 256×256 × 1 byte). Converting a page to colours, including the PS2 CLUT's swizzle and
  alpha, is not done here: that is rendering, MD4.
- The cells read on the MX vs. ATV terrains were only checked to exist (`Some`), not compared against
  that game's own terrain query. Bit-exactness on another game's terrain is MD3/MD8's question.

## Next

Nothing for MD1. MD4 decodes the pages through the palette for the ground.
