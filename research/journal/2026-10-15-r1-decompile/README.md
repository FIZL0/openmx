# R1 — the decompilation

2026-10-15. Task: `./kit plan R1`.

Ghidra 12.1.2 with `ghidra-emotionengine-reloaded` v2.1.38 over the one boot ELF: 2m57s, 8,558 functions,
`context/decomp/boot.{c,funcs.tsv,xrefs.tsv}`. The dump is at the right addresses, confirmed two ways —
every function lies inside the ELF's own code range, and the decompiled entry point zeroes exactly the bss
range the ELF headers declare. One program, because there are no overlays (D4).
