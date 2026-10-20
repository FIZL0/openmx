# R9 — r9 two players

2026-10-20. Task: `./kit plan R9`.

The original's two-player game, read from the decompilation and the disc. There are three split modes:
one view, horizontal (stacked) and vertical (side by side). cpresets.ini's suffixes are plain/h/v for
1P/2P-horizontal/2P-vertical at 4:3, and w/x are the 16:9 copies (w for 1P and the horizontal split,
x for the vertical). The q/z four-way slots exist in the loader and nowhere on the disc. Human i is pad
i. A two-player freeride loads a cut-down `_ss` area with no AI, shorter shadows and closer fog. The
HUD is per-player copies of each layer, plus a shared map and separator. `short_viewport` belongs to the
4:3 horizontal split, and the port should feed it per layout, not from the window's shape. Joust is
S37's contact clause, with nothing two-player in it. Where the second rider starts was not found, and
plan/HUMAN.md asks for two two-player states. Static reading only, nothing run. The names are in
research/oracle.txt's R9 section.
