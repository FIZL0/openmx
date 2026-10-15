# F3 — f3 vu0

2026-10-15. Task: `./kit plan F3`.

VU0 macro mode in `crates/ps2-float/src/vu.rs`: the ops this game actually uses, taken from a histogram
of every COP2 instruction inside a decompiled function (7,682 of them; 29 distinct ops once the
broadcast forms are folded in, and the I and R registers and the integer unit never appear), plus the state a sequence carries between instructions —
the accumulator, `Q`, and the MAC, status and clipping flags, which this game *branches on* rather than
merely setting. One method per instruction, so a ported routine reads line for line against its listing.
11 new tests, 72 passing in all; the op list also turned up three real bugs in the kit's own VU0
disassembler, now fixed. Nothing is proved against the original: the oracle is not built and the game
cannot boot yet, so the entry is PART and every modelling choice is listed in its
**Not verified / not 1:1** section.
