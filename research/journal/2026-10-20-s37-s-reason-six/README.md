# S37 — s reason six

2026-10-20. Task: `./kit plan S37`.

The wreck test's contact clause, ported: a rider whose head ray hits the scenery now wrecks at reason 6
(`air::hit_wreck`, `Wreck::Scenery`, the head ray swept by `ride::hits`). The clause is proved against the
original over 10,000 generated contact lists, and against recording2's own reason-6 wreck: the bike's
collision record, run over the RAM image saved on the tick boundary before it, finds the head ray against
no actor, and both the port and the original answer 6. The port sweeps only that one ray, once a frame;
see the file's Not verified section.
