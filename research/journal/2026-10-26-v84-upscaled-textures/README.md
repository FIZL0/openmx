# V84 — upscaled textures

2026-10-26. Task: `./kit plan V84`.

openmx can now draw upscaled or edited textures the way opentennis does, as its texture packs, key-named
files in `mods/texture-replacements/`, and mods' own `textures/upscaled/`. The new Upscaled Textures setting
(Graphics tab, on, a P126 departure) is the switch. There is now an `openmx --dump-textures` to write the
sources. opentennis's chaiNNer chain is copied byte for byte, and its `tools/upscale_mods.py` is ported to read
the PS2 textures a mod carries.

On free06, a ride with every one of its 83 dumped textures replaced by magenta drew the ground and props
magenta. The same with the bike and rider's 406 drew them green. With the setting off, both draw the disc's.

Two parts of opentennis's are left for follow-ups: the PCSX2 texture pack (V85), which cannot be proved
against this game's GS uploads yet, and live reload (V86). Details are in 1-upscaled-textures-FINAL.md.
