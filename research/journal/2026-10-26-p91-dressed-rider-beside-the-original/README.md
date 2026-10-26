# P91 — dressed rider beside the original

2026-10-26. Task: `./kit plan P91`.

P86's dressed rider set beside a PCSX2 custom rider named "Maximilianus" with number 888, on a 125
(white-digit plate) and a CR250_THQ_01 (dark-digit plate). The original does not fit a long name: it
centres the string at scale 1 and paints by a row-major index with no x clip, so the overflow wraps onto
the rows above and below. The port now paints the same way. A pro rider's gear set takes the [JerseyPants]
entry its .sub's jersey and pants renames match, not name1/number1. Each bike's plate font is the one in
its own record. The port's NameDefault texture matches the one captured from RAM texel for texel in
coverage, and every colour is within 4 (rgb) and 2 (alpha). Details in 1-….
