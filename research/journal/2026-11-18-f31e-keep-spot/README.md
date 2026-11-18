# F31e — keep spot

2026-11-18. Task: `./kit plan F31e`.

The bike's put-back spot (+0xd38..+0xd6c) is kept by the run's update after the sim frame. Ported as
`handling::spot`; the pure-Rust flip replay now keeps the spot itself instead of poking it from the
rows, and stays bit-exact on all 465 rows with no interpreter instruction. Details in 1-keep-spot.
