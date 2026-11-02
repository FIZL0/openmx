# MD68 — the mod store takes the world mods' mod.json

2026-11-02. Task: `./kit plan MD68`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

Uploading or installing world_mxu, world_mxvatv, world_untamed or world_ultra failed with "`mod.json`
is too big". The cause was `ZipDir::check_mod_json` reading the zip's `mod.json` under `MAX_META`
(64 KiB), the cap meant for the store's `meta.json` listing. A world's `mod.json` runs from 94 KB to
476 KB, nearly all of it the joins list. It now has its own cap, `MAX_MOD_JSON` = 1 MiB, and
`meta.json` keeps 64 KiB. Done; details in `1-big-mod-json-FINAL.md`.
