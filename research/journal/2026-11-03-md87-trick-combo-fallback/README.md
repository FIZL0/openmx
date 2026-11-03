# MD87 — a trick combo falls back to the trick without its modifier

2026-11-03. Task: `./kit plan MD87`.

The user's brief: "when we are holding r1 and do a trick its not doing any with cross and square, only
override if there is a new trick to be done, otherwise fall back to the non modified trick".

A gesture now starts its combination's trick only when a pack put one in that slot. Otherwise it drops
the held modifier and starts what is left: R1 + Cross starts Cross's trick.

See [1-trick-combo-fallback-FINAL.md](1-trick-combo-fallback-FINAL.md).
