# 1 — xbox texture picks (PART)

Task P383a. 2026-11-22.

## What I did

- Unpacked the Xbox disc again in the scratchpad (the P383 copy was gone): `tools/xbox_xiso.py ISO x X`,
  then `tools/xbox_res.py FILE x R/<path less .res>` on each of the 520 `.res` → 9,565 `.dxt`, as P383.
  PS2 side: the user's `~/repos/openmx/mods/textures-src` (10,682 PNGs from `openmx --dump-textures`).
- **Decoder fix**: Xbox format 0x37 (8-bit palette) had its alpha forced to 255. Its palette alpha is real:
  on the 1,067 same-size pairs whose Xbox file is 0x37, 1,061 match the PS2 picture's alpha within 4 (mean
  per pixel) and the other 6 (free09/free11 `*_mask`) within 14; the 18 files whose palette alpha is all 0
  (`namedefault*`) are all-transparent on the PS2 too. Forcing 255 is what made the 0x37 logos and fonts
  top the first scoring at 255 "apart". `tools/xbox_textures.py` now keeps it; the test fixture
  (`crates/openmx/tests/xbox_textures.rs`) is made opaque to match; research/mxu-xbox.md corrected. Kept in
  this task (the spec review flagged it as unasked): without it the page would lead with ~200 logos whose
  only "difference" is the decoder's lost alpha, and a later pick of a 0x37 picture would pack it opaque.
- Spec review fixes: clicking a card's name toggled twice (label + handler), now once; the 16 scoring regions
  are even (`np.array_split`; a leftover strip could make more); `differ`'s default cut-off is 12, the page's.
- New `xbox_textures.py differ TEXTURES_SRC XBOX_DIR OUT [--least D]` (needs PIL: `uv run --with pillow
  --with numpy`): scores each same-size pair by how different it looks (both laid on grey by alpha,
  averaged over 4×4 cells so DXT block noise and palette dither count little; mean colour distance plus
  half the alpha distance; the worst of 16 regions), one card per file stem (as `KEEP` names them, the most
  different archive's pair), into `OUT/differ.tsv` and a pick page `OUT/index.html` + `OUT/img/`.
- P383's 8,998 same-size pairs (width and height equal) → 2,142 distinct stems (many files repeat across
  archives).
  Scores: 785 stems ≥ 8, 345 ≥ 10, 116 ≥ 12, 60 ≥ 15, 48 ≥ 20. Looked at samples by eye: 9–11 look the
  same (a touch of contrast); 12–15 mostly the same, some clearly recoloured (lodgepole); ≥ 15 real
  changes (repaints, different art: stadiumemptyseats, bales_75_77 with Honda logos, monstertruck_doors
  with art where the PS2 is a grey see-through placeholder, controller buttons with the Xbox's buttons).
  Cut-off 12: **115 cards** on the page.
- Page published (private): https://claude.ai/artifact/S9rKCXhT3uSYenyHmnk28Z — tick the Xbox pictures,
  Send writes `P383a picks: keep <stem> …` to the page's db (`picks/P383a`), as `./kit sheet` pages do.
  Not a `./kit sheet` pick sheet: those carry no pictures, and the choice is only visible in them.
- Scores of every stem: `~/repos/openmx/context/p383/differ.tsv` (score, stem, rel, PS2 key, archives).
- `pack` rerun with the fixed decoder: 9,424 pairs, 376 kept, byte-identical to both
  `mods/xbox-textures` (this worktree) and `~/repos/openmx/mods/xbox-textures` (`diff -rq`: nothing). No kept
  picture is a 0x37 with alpha, so the fix changes no pack file yet.
- opentennis: nothing to take (no texture-pack picker there).

## What the original does

Not a simulation task: the Xbox disc is only a texture source; the PS2 game is unchanged with the pack off.

## How it was proved

- The 0x37 alpha: compared against the PS2 picture's alpha on all 1,067 same-size 0x37 pairs (numbers above).
- The cut-off: by eye on montages of 12 pairs from each score band (9–11, 12–20, 15–40, ≥ 40).
- `./kit check -- -p openmx --test xbox_textures` for the fixture change.

## Not verified / not 1:1

- **The user's picks**: not made (unattended run). `KEEP` is still empty and the pack is unchanged; the task
  blocks on DECIDE with the page link.
- **Pairs under the cut-off (score < 12, 2,027 stems)**: not on the page; samples at 9–11 looked the same,
  but not every one was looked at. `differ --least 8` puts 785 on a page.
- **Alpha meaning on models**: a low PS2 alpha on a model texture (monstertruck_doors, mm_dumptruck) may be
  a shine mask, not a hole; the page shows it as see-through. Whether an Xbox pick with different alpha
  draws right in the game is not checked.
- **0x38 alpha**: unchanged (it already kept alpha); not compared against the PS2 here.
- **`differ` has no automated test**: it needs PIL, which the gate's python lacks; checked by running it on
  the whole disc.

## Next

When the picks come back (reopened with `P383a picks: keep …` in the note): put the stems in `KEEP` in
`tools/xbox_textures.py`, unpack the Xbox disc as above (or reuse an unpacked copy), and run
`tools/xbox_textures.py pack ~/repos/openmx/mods/textures-src <xbox R dir> mods/xbox-textures` and the same
into `~/repos/openmx/mods/xbox-textures`; check the shell reads it (`textures: N from 1 texture packs`).
