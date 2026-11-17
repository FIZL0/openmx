# F60 — sweep follows listener query

2026-11-16. Task: `./kit plan F60`.

F51 left 2 of 1,500 global sweep searches and 2 of 600 nearby sweeps skipped: a struck plant part's
listener runs a database query of its own in the middle of the walk. Now both oracles run every case.
The global search lends the database to its caller round each test and takes it back after
(`atv_sweep::Outside::lend` / `take_back`). The nearby sweep never needed a change: its walk is its
list's, which the listener's query leaves alone. Details in `1-…-FINAL.md`.
