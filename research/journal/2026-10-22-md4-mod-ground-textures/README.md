# MD4 — mod ground textures

2026-10-22. Task: `./kit plan MD4`.

A mod track (MD2) with no splat now draws its ground from its terrain's own texture pages, the way MX
vs. ATV does: each 4x4 block of leaves takes the page its parent's table names, inside the pages'
8-pixel border (proved on all 56 MX vs. ATV and 49 Untamed terrains by exact seam agreement), with the
map's detail texture blended over it at the scene's `DetailMapTilingMultiplier` (by eye, not 1:1). Both
renderers; MXU's ground is unchanged. Details in the numbered file.
