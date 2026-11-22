# 1 — xbox texture pack survey (FINAL)

Task P383. 2026-11-21. Follows research/journal/2026-11-20-p383-xbox-texture-pack/ (the tools, made while
the Xbox iso was missing). The iso is back at the repo root, so this session made the survey and the pack.

## What I did

- Unpacked the Xbox disc: `tools/xbox_xiso.py … x X`, then `tools/xbox_res.py` on each of its 520 `.res`
  (a shell loop in the scratchpad, every one unpacked) → 9,565 `.dxt`. `openmx --dump-textures` of the PS2
  disc → 10,786 PNGs.
- `tools/xbox_textures.py pack`: 9,400 pairs, **376 Xbox pictures kept** (more pixels), 8,974 the same
  size, 50 smaller, 1,258 PS2 textures with no Xbox twin. Counts by archive and the formats are in
  research/mxu-xbox.md "The texture pack (P383)". `free06_grass` 128 → 256 and `mill_siding` 128 → 256
  are in; most of the rest are the 255 track previews (256 → 512) and 27 bike chrome pictures (64 → 256).
- The pack is `mods/xbox-textures/` (52 MB, git-ignored, game data) in this worktree and in the user's
  mods folder `~/repos/openmx/mods/xbox-textures/`; the shell reads it there:
  `textures: 274 from 1 texture packs` (274 keys: 17 keys sit in more than one archive, each with one
  identical Xbox picture, checked by md5).
- `pack` was ~20 minutes (the DXT decoder ran block by block in Python). Now it reads each Xbox picture's
  size from its headers (`dxt_size`) and decodes only the kept ones, and `dxt_blocks` decodes all blocks at
  once with numpy: 33 s. Same output as the slow run (`diff -r` of the two packs: identical), and the new
  decoder equals the old one on 156 sampled files of every format the old one read (0x26, 0x36, 0x37, 0x38).
- `upscale_mods.py --disc --xbox`: unchanged since the 11-20 entry; not run here (it needs an upscale model,
  and the task asks only that it can take the Xbox picture, which its test proves).
- Follow-up filed: P383a (low) — the user's picks among the same-size pairs, and the 0x32 format.
- opentennis: nothing to take (as on 11-20).

## What the original does

Not a simulation task: the Xbox disc is only a texture source; the PS2 game is unchanged with the pack off.

## How it was proved

- `crates/openmx/tests/xbox_textures.rs` (5 tests pass, `./kit check -- -p openmx --test xbox_textures`):
  a pack PNG keyed to free06_grass's PS2 key replaces it, and the pack tool keeps the bigger Xbox picture
  under the PS2 key.
- By eye, free06 with the pack on and off (`--textures upscaled` / `original`, `--mods` the pack), shots in
  `context/p383/` (git-ignored):
  - `free06-grass-pack-on.png` / `-off.png` (`--shot-pose 2884,352,2750,0,-35,60`, near the start, looking
    down at the grass): with the pack on, the grass slope is greener and has more detail; the dirt
    track is the same.
  - `free06-start-pack-on.png` / `-off.png` (the default start shot): hardly any visible difference at that
    distance; the files differ.
  - `context/p383/survey.tsv`: every pair and why it was kept or not.

## Not verified / not 1:1

- **"Or the user judges it better"**: `KEEP` is still empty. Nobody looked at the 8,974 same-size pairs
  (unattended run); filed as P383a.
- **24 format-0x32 Xbox files** (`Tracks/sx*/tex/score_tower4`): skipped, the decoder does not know their
  mip layout. The PS2 picture stays for them.
- **Kept pictures other than free06_grass seen in the game**: only the free06 grass was looked at in a
  ride. The track previews, bike chrome and mill pictures were checked only by size and the md5 check above.
- **Terrain ground pages and `.atm` animations**: not matched (the Xbox `.dx.trn` is still undecoded).
  free06's grass shows because the freeride ground paints its layers from the replaceable `free06_grass`
  texture.
- **The upscale with the Xbox source**: not run (no upscale model run here); only `--check` is proved.
- **Alpha of DXT1 punch-through and DXT5 on real files in the game**: decoded as the 10-31 decoder does,
  not compared against the Xbox itself (no Xbox emulator here).

## Next

P383a: the user picks Xbox pictures they like better among the same-size pairs.
