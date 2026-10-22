# 1 — nat terrain (FINAL)

Task S98. 2026-10-22.

## What I did

- `game_sim::track::Terrain::parse` now reads the textures a `.trn` stores between its blocks and its
  tree, and keeps them as `Terrain::textures` (`TerrainTexture` / `TextureLevel`, borrowed slices). The
  free tracks store none, so the check that failed on every national was right for them and wrong for
  the nationals: nat19's 538 blocks end at 1148779, and the 2097920 bytes from there to the tree are
  32 × 65560, its 32 textures.
- The two header words the parser called `textures` and `_textures_again` are a table length and a
  stored count. The table holds the file offset of each texture. The original skips it. The parser checks
  each offset against where the texture really starts, and that the last texture ends where the tree
  begins.
- Tests: `crates/game-sim/tests/track.rs` `every_tracks_terrain_accounts_for_its_blocks_and_textures`
  reads every track archive's `.ps2.trn` on the disc. All 22 nationals parse, each with 32 or 48
  textures of format 39, one level of 256×256 bytes. The free tracks have none, and every leaf's block is
  in range. `crates/game-render/tests/ecosystem.rs` `nat19s_terrain_reads_and_its_plants_stand_on_it`
  checks that nat19's extent is 5760 (5 roots × 16² × 4.5) and that all 12785 plants ask to be stood.
  Every plant is inside the map, and they reach past 5747 on both axes. The plants' y, and every model and
  billboard base, sit 0 to −0.088 ft from the sim's ground, which is under one 65536th of the map, the same
  bound as free06. The existing every-scene test no longer tolerates a terrain that fails to read.
- opentennis has no terrain format, so nothing was taken from it.

## What the original does

The terrain loader (`./kit fn 0x1d9930`, the version-'7' path near the end) reads, in order: scale, levels,
tree offset, the per-block word count, and the block count. It then skips the per-block words. Next it reads
the texture-table length and the texture count, and skips the table (length × 4 bytes). It reads the blocks
one by one, then, for each texture, a format word followed by a texture object's own read through its
vtable's +0x28 slot (`./kit fn 0x187f80`). That read takes a level count, then for each level
(`./kit fn 0x188160`) a 16-byte header of width, height, byte count and a fourth word. For formats 39/40 the
byte count is followed by exactly that many bytes. After the textures it reads the tree. On nat19 the
header is `39 | 1 | 256 256 65536 0x407f23`. The fourth word changes from file to file and looks like a
pointer the authoring tool left behind.

## How it was proved

A structural proof against the user's own disc: every `.ps2.trn` must account for itself to the byte. Each
texture starts where its table entry says, and the last one ends where the tree begins, on all 22 nationals.
The plant check reuses V48's method (the sim's own terrain under each `.esb` plant).
`./kit check` passes.

## Not verified / not 1:1

- **No nationals save state.** No nat track has been loaded in PCSX2. So nat19's plant words and its
  ecosystem scale (+0xd4) were not compared against the original's RAM the way V48 compared free06's.
  "Plants stand on it" is proved against the sim's terrain and the extent formula V48 proved on free09.
  All of nat19's plants are snapped, so their y comes from that same terrain by construction. The
  independent part is the x/z spread, which fills the 5760 map and stays inside it.
- **Texture formats other than 39/40 are refused.** The texture reader takes formats 33/34 another way,
  through a palette path that was not read, and it has a flag (bit 3 of its +200 byte) that changes where
  the byte count comes from. No terrain on this disc uses either, so neither is ported.
- **Mip chains:** the texture reader skips levels after the first unless flag 2 is set. Every texture
  here has one level, so the parser just reads every level whole. That has not been checked on a
  multi-level texture.
- **What the textures are for, and how the pixels map** (palette indices into the file's 1024-byte palette
  is the guess) was not read. Nothing draws them yet. The nationals' ground in the renderer is not part of
  this task.
- **The nationals are not ridden.** This makes their terrain parse. It does not prove the sim's ground
  queries or the shell on a national track.

## Next

Nothing for S98.
