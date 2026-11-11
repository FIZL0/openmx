# S281 — s12 ai tick

2026-11-10. Task: `./kit plan S281`.

This task ported the AI brain's per-tick update (`ai::update`): the original's order, the type-1 climb rule, the stuck clock, the lever hand-off to the bike (interface vt+0xc) and the rider assist. Each was oracled over 10,000 cases, and a free09 capture of the three AI riders was replayed bit for bit. The callouts it reaches through `ai::Steps` are still unported; they are filed as S312. Details are in 1-s12-ai-tick-FINAL.md.
