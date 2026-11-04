# R12 — r11 names

2026-11-04. Task: `./kit plan R12`.

R11's (and later tasks') `pc_`/`atv_` rows in research/oracle.txt carried into the two reference Ghidra
projects with `./kit ghidra name -o pc` and a first `./kit ghidra analyse -o atv` on the folded
`atv_game.elf`. All 120 pc rows and 302 of the 313 atv rows are function names; the other 10 atv rows are
labels (data, vtables, three never-disassembled thunks), and `atv_world` is a heap address, outside the
ELF. NameOther now makes a function where a row sits on an instruction with no function, and a label
otherwise. The projects live in s3's git-ignored context/ only. Details in 1-r11-names-FINAL.md.
