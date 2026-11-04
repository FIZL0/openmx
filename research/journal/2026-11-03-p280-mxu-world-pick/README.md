# P280 — mxu world pick

2026-11-03. Task: `./kit plan P280`.

The user found no MX Unleashed world in the track pick. The disc's three worlds (`menu::DISC_WORLDS`)
were always rows, but only in **All Games** and **MX Unleashed**. The **Worlds** filter, the one that
holds every installed world (world_mxvatv, world_ultra, world_untamed), is a range of mod rows, and the
disc's worlds sit in the disc's block before the mods, so that filter never listed them. The installed
world_mxu and world_mxu_free are merged into the disc's rows (P232), so they don't show there either. The
fix: the Worlds filter now lists the disc's worlds first, then the installed ones. Details in
1-mxu-world-pick-FINAL.md.
