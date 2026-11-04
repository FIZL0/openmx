# F31b — SLUS-21104's bike sim, split; F31b1: the bike's PreProcess

2026-11-03. Task: `./kit plan F31b`.

F31b is the whole per-frame bike sim of MX vs. ATV: too much for one session. It is split
into F31b1..F31b5 (PreProcess; Force body; PostProcess; record pass, tail, wreck and put-back;
the whole frame in pure Rust). This session did F31b1: the bike's PreProcess (ground head) is ported
as `handling::bike::ground_head`. It matches the original on 12 000 random bikes, and it replaces the
original in F31a's hybrid frame for every row of C15 ride-2 and flip-1, which stay bit-exact.
Details in 1-bike-pre-process-FINAL.md.
