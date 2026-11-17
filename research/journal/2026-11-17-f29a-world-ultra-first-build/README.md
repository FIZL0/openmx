# F29a — world_ultra's first ground build

2026-11-17. Task: `./kit plan F29a`.

The ground's first build now runs on 8 threads at most instead of one per core. On the 32-core
desktop world_ultra's peak fell from 1619–1716 MB to 1350–1445 MB; the first frame comes ~0.4 s
later (ground build 1.7 → 2.1–2.4 s) at load 7–9. The Deck already had 8 threads.

Code: `crates/openmx/src/worlds.rs` (`FIRST_BUILDERS`).

Entry: [1-world-ultra-first-build-FINAL.md](1-world-ultra-first-build-FINAL.md).
